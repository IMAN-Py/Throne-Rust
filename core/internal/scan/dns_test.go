package scan

import (
	"testing"

	"ThroneCore/internal/fastcore"
	"golang.org/x/net/dns/dnsmessage"
)

func parseXDNSReplyGoReference(packet []byte, query xdnsQuery) (xdnsReply, bool) {
	var parser dnsmessage.Parser
	header, err := parser.Start(packet)
	if err != nil || !header.Response || header.ID != query.id {
		return xdnsReply{}, false
	}
	question, err := parser.Question()
	if err != nil {
		return xdnsReply{rcode: header.RCode}, err == dnsmessage.ErrSectionDone && header.RCode != dnsmessage.RCodeSuccess
	}
	if !sameDNSName(question.Name, query.name) {
		return xdnsReply{}, false
	}
	reply := xdnsReply{rcode: header.RCode}
	if parser.SkipAllQuestions() != nil {
		return reply, true
	}
	for {
		answer, err := parser.AnswerHeader()
		if err != nil {
			return reply, true
		}
		if answer.Type == dnsmessage.TypeTXT && answer.Class == dnsmessage.ClassINET && sameDNSName(answer.Name, query.name) {
			reply.txt = true
			return reply, true
		}
		if parser.SkipAnswer() != nil {
			return reply, true
		}
	}
}

func buildTestDNSPacket(id uint16, response bool, rcode dnsmessage.RCode, qName, aName string, withTXT bool, compress bool) []byte {
	hdr := dnsmessage.Header{
		ID:       id,
		Response: response,
		RCode:    rcode,
	}
	b := dnsmessage.NewBuilder(nil, hdr)
	if compress {
		b.EnableCompression()
	}
	if qName != "" {
		_ = b.StartQuestions()
		_ = b.Question(dnsmessage.Question{
			Name:  dnsmessage.MustNewName(qName),
			Type:  dnsmessage.TypeTXT,
			Class: dnsmessage.ClassINET,
		})
	}
	if withTXT && aName != "" {
		_ = b.StartAnswers()
		_ = b.TXTResource(
			dnsmessage.ResourceHeader{
				Name:  dnsmessage.MustNewName(aName),
				Type:  dnsmessage.TypeTXT,
				Class: dnsmessage.ClassINET,
				TTL:   30,
			},
			dnsmessage.TXTResource{TXT: []string{"hello-xdns"}},
		)
	}
	out, _ := b.Finish()
	return out
}

func TestParseXDNSReplyDifferential(t *testing.T) {
	if !fastcore.Available() {
		t.Fatalf("throne_fast_core.dll not loaded")
	}

	qName := dnsmessage.MustNewName("a1b2c3d4e5.xdns.example.com.")
	q := xdnsQuery{id: 0x3A7F, name: qName}

	cases := []struct {
		name string
		pkt  []byte
	}{
		{"valid_compressed_txt", buildTestDNSPacket(0x3A7F, true, dnsmessage.RCodeSuccess, "a1b2c3d4e5.xdns.example.com.", "a1b2c3d4e5.xdns.example.com.", true, true)},
		{"valid_0x20_case_randomized", buildTestDNSPacket(0x3A7F, true, dnsmessage.RCodeSuccess, "A1b2C3d4e5.XdNs.ExAmPlE.cOm.", "a1b2c3D4E5.xdns.example.COM.", true, false)},
		{"wrong_id", buildTestDNSPacket(0x1111, true, dnsmessage.RCodeSuccess, "a1b2c3d4e5.xdns.example.com.", "a1b2c3d4e5.xdns.example.com.", true, true)},
		{"not_a_response", buildTestDNSPacket(0x3A7F, false, dnsmessage.RCodeSuccess, "a1b2c3d4e5.xdns.example.com.", "", false, false)},
		{"nxdomain_no_question", buildTestDNSPacket(0x3A7F, true, dnsmessage.RCodeNameError, "", "", false, false)},
		{"noerror_no_question", buildTestDNSPacket(0x3A7F, true, dnsmessage.RCodeSuccess, "", "", false, false)},
		{"noerror_empty_answer", buildTestDNSPacket(0x3A7F, true, dnsmessage.RCodeSuccess, "a1b2c3d4e5.xdns.example.com.", "", false, false)},
		{"mismatched_question_name", buildTestDNSPacket(0x3A7F, true, dnsmessage.RCodeSuccess, "other.example.com.", "a1b2c3d4e5.xdns.example.com.", true, false)},
		{"mismatched_answer_name", buildTestDNSPacket(0x3A7F, true, dnsmessage.RCodeSuccess, "a1b2c3d4e5.xdns.example.com.", "other.example.com.", true, false)},
		{"truncated_short", []byte{0x3A, 0x7F, 0x80}},
		{"compression_loop", []byte{0x3A, 0x7F, 0x80, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0, 0xC0, 12, 0, 16, 0, 1}},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			gotReply, gotMatch := parseXDNSReply(tc.pkt, q)
			wantReply, wantMatch := parseXDNSReplyGoReference(tc.pkt, q)
			if gotMatch != wantMatch || gotReply != wantReply {
				t.Fatalf("mismatch for %s: rust=(%+v, %v) go=(%+v, %v)", tc.name, gotReply, gotMatch, wantReply, wantMatch)
			}
		})
	}
}

func BenchmarkParseXDNSReplyRust(b *testing.B) {
	if !fastcore.Available() {
		b.Skip("throne_fast_core.dll not loaded")
	}
	qName := dnsmessage.MustNewName("a1b2c3d4e5.xdns.example.com.")
	q := xdnsQuery{id: 0x3A7F, name: qName}
	pkt := buildTestDNSPacket(0x3A7F, true, dnsmessage.RCodeSuccess, "a1b2c3d4e5.xdns.example.com.", "A1B2c3d4e5.XDNS.example.com.", true, true)
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		r, ok := parseXDNSReply(pkt, q)
		if !ok || !r.txt {
			b.Fatal("unexpected mismatch")
		}
	}
}

func BenchmarkParseXDNSReplyGo(b *testing.B) {
	qName := dnsmessage.MustNewName("a1b2c3d4e5.xdns.example.com.")
	q := xdnsQuery{id: 0x3A7F, name: qName}
	pkt := buildTestDNSPacket(0x3A7F, true, dnsmessage.RCodeSuccess, "a1b2c3d4e5.xdns.example.com.", "A1B2c3d4e5.XDNS.example.com.", true, true)
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		r, ok := parseXDNSReplyGoReference(pkt, q)
		if !ok || !r.txt {
			b.Fatal("unexpected mismatch")
		}
	}
}
